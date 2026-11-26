# Pastora engineering audit (v0.1.3, commit b1333f5)

Scope: every source file in `src-tauri/src` (Rust, ~3.9k lines), `src` (React/TS, ~3.9k lines), Tauri config,
capabilities, CI and release workflows. Baseline: `cargo test` 12/12 pass, `npm run build` passes, clippy 3 style
warnings, `npm audit` 0 vulnerabilities.

## 1. Architecture

```
            ┌───────────── Rust process (Tauri 2) ─────────────────────────────┐
 OS pasteboard ◄─poll 350ms── watcher thread ──► history.rs ──┐                 │
 global hotkeys ─► shortcuts.rs ─► window.rs / actions.rs     │  Mutex<rusqlite::Connection>
 tray menu ─────► tray.rs                                     ├─► clipboard.db (WAL, sync=FULL)
 background thread (2s): auto-lock, pause expiry, AX state    │   clips · pins · vault_meta · vault_entries · settings
 updates thread (daily): tauri-plugin-updater                 │
 commands.rs (≈55 IPC commands) ◄──── invoke ────┐            │   images/<sha256>.<ext> + .thumb.png
 vault/: Argon2id KEK wraps random 256-bit key;  │            │
         XChaCha20-Poly1305 per entry, id as AAD │            │
 keychain.rs: vault key in OS keychain (Touch ID)│            │
            └────────────────────────────────────┼────────────┘
                                                 │
   WebViews: "popup" (History / Pins / Secure tabs) and "settings" — React 19, Tailwind 4
```

State: `AppState` (one `Mutex<Connection>`, `RwLock<Settings>`, `Mutex<Option<Key>>`, atomics for pasteboard
change counts). Sync commands run on the main thread; `async` ones on the Tokio pool.

## 2. Feature inventory and health

| Feature | Health |
|---|---|
| Clipboard capture (text, files, image files, raw images), dedupe, ignore apps/patterns, pause | Works. Linux polling is costly (R-01). |
| History list/search/paging, paste/copy, transforms, view, delete, clear, limits | Works. |
| Pins: add/edit/reorder/import/export | Works. Import accepts arbitrary kinds, not atomic (P3-04). |
| Secure Clips: setup/unlock/Touch ID/lock/auto-lock/templates/TOTP/generator/backup | Mostly works. Bugs P1-02…P2-04, P3-01/02. |
| Username→password sequence, secret auto-clear, concealed/current-host-only marking | Works (macOS). Linux has no concealed marker (R-05). |
| Global shortcuts, tray menu, popup placement, settings window | Works, but tray can deadlock (P1-01). |
| Auto-update (signed) | Works. Holds package in RAM (R-02). |

## 3. Confirmed findings (verified by reading the code; tests added where feasible)

### P1-01 · Concurrency · Tray refresh can deadlock the whole app
- `src-tauri/src/tray.rs:98-129`
- `refresh()` holds `st.tray` (Mutex) while calling `set_checked` / `menu.insert` / `menu.remove`. Off the main
  thread, Tauri implements these as `run_on_main_thread` + blocking `rx.recv()` (tauri-2.12.5 `src/menu/mod.rs:26`).
  `refresh()` is called from the background thread (pause expiry, Accessibility change), the updater (download
  progress), and from the main thread (sync commands `set_settings`, `pause_recording`, tray menu events).
- Manifestation: background thread holds the lock and waits for the main thread; the main thread (user changing a
  setting) waits for the lock → permanent freeze of UI, hotkeys and tray. Low probability, total impact.
- Fix: always run the refresh body on the main thread (`run_on_main_thread`), so the lock is never held while
  waiting for it. Complexity: low. Verify: build; code inspection (needs a GUI runtime to reproduce).

### P1-02 · Security/DoS · Vault backup import trusts Argon2 parameters from the file
- `src-tauri/src/vault/mod.rs:353-359`
- `open_backup` derives with `m_cost/t_cost/p_cost` taken from the JSON. A crafted backup with `mCost: 4294967295`
  makes Argon2 try to allocate ~4 TiB → abort, or `tCost: 4e9` hangs a worker forever.
- Fix: bound parameters (m 8 MiB–1 GiB, t 1–10, p 1–8) before deriving. Test: `backup_rejects_absurd_kdf_params`.

### P2-01 · Correctness · TOTP ignores `digits`, `period` and `algorithm` from otpauth URIs
- `src-tauri/src/vault/totp.rs:5-34`, `src/components/vault/EntryDetail.tsx:40`
- Any `otpauth://…&digits=8`, `period=60` or `algorithm=SHA256` secret silently produces wrong codes.
- Fix: parse and honour the three parameters (SHA1/256/512, 6–8 digits, 1–300 s); UI countdown uses the period.
  Test: RFC 6238 SHA256/SHA512 8-digit vectors.

### P2-02 · Data loss · Saving an entry that no longer exists reports success and drops the data
- `src-tauri/src/vault/mod.rs:258-264`
- `UPDATE … WHERE id = ?` affecting 0 rows (entry deleted from another place while the editor was open, or after a
  vault reset) returns `Ok(id)`; the UI shows "Saved" and the content is gone.
- Fix: if no row was updated, insert it. Test: `save_recreates_missing_entry`.

### P2-03 · Consistency · Vault import is not atomic
- `src-tauri/src/vault/mod.rs:367-377`
- A corrupt entry halfway through leaves earlier entries imported; retrying duplicates them. Also one fsync per
  entry (synchronous=FULL). Fix: one transaction. Test: `import_is_all_or_nothing`.

### P2-04 · Performance/Concurrency · Argon2 runs while holding the database lock; setup can race
- `src-tauri/src/commands.rs:237,248,336`, `vault/mod.rs:139-184`
- `vault::unlock(&st.db(), …)` keeps the DB mutex for the whole KDF: measured **125 ms** per attempt
  (release, this Mac), ~250 ms for a password change, blocking clipboard capture and every list query.
  Two concurrent `vault_setup` calls could both pass `is_initialized` with the lock split.
- Fix: read metadata under the lock, derive without it, write with `INSERT` (setup) / `UPDATE` (password change).

### P2-05 · Performance · Screenshot thumbnail decodes the PNG it just encoded
- `src-tauri/src/watcher.rs:197-223`, `history.rs:128-132`
- For clipboard images the RGBA pixels are already in memory, but the thumbnail is made by decoding the freshly
  encoded PNG. Measured on a 5120×2880 image: **243 ms** (decode PNG) vs **96 ms** from RGBA incl. a 59 MB copy
  that the real fix avoids. Also ~60 MB less transient allocation.
- Fix: build the thumbnail from the RGBA buffer (borrowed, no copy).

### P3-01 · Efficiency/Security · TOTP field polls the backend every second
- `src/components/vault/EntryDetail.tsx:20-33`
- 1 IPC + DB read + decrypt per second for a value that changes every 30 s; every call also resets the vault
  idle timer. Fix: fetch once per period (at `remaining` seconds).

### P3-02 · Bug · Password generator "Dashes" toggle does nothing
- `src/lib/password.ts:25` — `join(o.symbols ? "-" : " ").replace(/ /g, "-")` always yields dashes.
- Fix: no separator when off.

### P3-03 · Robustness · `Mutex::lock().unwrap()` on security-critical state
- `state.rs:82,87`, `actions.rs`, `commands.rs`, `tray.rs`, `shortcuts.rs`, `updates.rs`
- A panic while one of these locks is held poisons it; the next `unwrap()` in the background thread kills it and
  **auto-lock silently stops**. Fix: recover poisoned guards like `AppState::db()` already does.

### P3-04 · Validation · Settings/pins import accepts any `kind` and isn't atomic
- `src-tauri/src/commands.rs:652-668` — a crafted file can add `kind: "image"` pins with no image (unpastable).
- Fix: only `text`/`files`, one lock + transaction.

## 4. Risks and opportunities (not fixed; need a decision, a platform to test on, or are design trade-offs)

| ID | Sev | Where | Issue | Recommendation |
|---|---|---|---|---|
| R-01 (fixed, §8) | P1 (Linux) | `platform/generic.rs:39-58` | Linux has no change counter, so every 350 ms the watcher reads the whole clipboard and, for images, decodes the full image to hash it. A copied 4K screenshot means continuous decode work, likely tens of percent of a core. Not measured (no Linux host). | Use XFixes selection-owner events (x11rb) or slow down image fingerprinting; measure with `perf` first. |
| R-02 (fixed, §8) | P2 | `updates.rs:55,146` | Downloaded update (~10–20 MB) stays in RAM until restart. | Write it to the cache dir and read it back in `install`. |
| R-03 (fixed, §8) | P2 | `tauri.conf.json` `"csp": null` | No CSP. React escapes everything and there's no `innerHTML`, but any future XSS would reach IPC (`vault_reveal_field`, `vault_export(path)` writes anywhere). | Set `default-src 'self'; img-src 'self' asset: http://asset.localhost data:; style-src 'self' 'unsafe-inline'; connect-src ipc: http://ipc.localhost` and verify in a release build. |
| R-04 (fixed, §8) | P2 | `keychain.rs` | Touch ID is an app-level gate: the keychain item has no biometric access control, so the key is readable by this app's signature without biometrics. Ad-hoc signing (`"signingIdentity": "-"`) changes the signature every release. | Store with `kSecAccessControlBiometryCurrentSet` once signed with a stable identity. |
| R-05 | P3 | `generic.rs:116-133` | Linux: copied secrets get no concealed marker, so other clipboard managers record them. | Document; add `x-kde-passwordManagerHint` MIME. |
| R-06 | P3 | `vault/mod.rs` | Decrypted `Entry` strings aren't zeroized (README promises zeroization of the key only, which is true). | `Zeroizing<String>` for field values if desired. |
| R-07 | P3 | `updates.rs:227` | `thread::sleep(DAY)` doesn't advance during system sleep on macOS, so "daily" can be several days; enabling the setting later waits for the next cycle. | Sleep in short steps, compare wall-clock timestamps. |
| R-08 | P3 | `Settings.tsx:32`, `commands.rs:499` | Settings are written as a whole object from the window's copy: two changes within one IPC round trip can undo each other. | Send patches. |
| R-09 | P3 | `EntryEditor.tsx:239` | Field editors keyed by index: removing a field moves its "show password" state to the next one. | Key by a stable client id. |
| R-10 | P3 | `generic.rs:249`, `settings.rs:94` | Windows/Linux: "Ignored apps" picker is empty and the defaults are macOS bundle ids. | Enumerate processes on Windows; ship exe-name defaults. |
| R-11 | P3 | `history.rs:89` | Search is `LIKE '%q%'` (full scan). Fine at default 500 items; with "unlimited" history and large clips it holds the DB lock per keystroke. | FTS5 if users report slowness. |
| R-12 | P3 | workflows | Actions pinned by tag, not SHA, in a job holding the updater signing key. | Pin to commit SHAs. |
| R-13 | P3 | `watcher.rs:250` | PNG encode of a 5K screenshot: 514 ms at default compression (off the main thread, without the DB lock). | `CompressionType::Fast` trades CPU for larger files (counts against the size limit); leave unless users complain. |
| R-14 | info | `Popup.tsx:192` | The Secure tab unmounts when the popup hides, so an unsaved entry is lost if you click away. Intentional (drops secrets), but surprising. | Consider warning or keeping non-secret drafts. |

## 5. Test gaps

No frontend tests at all; no tests for watcher capture, commands, tray/window, updater, shortcut parsing,
otpauth parsing variants, backup import failure paths, or concurrent unlock/setup. Highest value additions:
vault failure paths (added), TOTP vectors (added), `shortcut.ts` accelerator conversion, `password.ts` generator
character-set guarantees, and a Linux CPU soak test for R-01.

## 6. Changes implemented

| Finding | Change | Files |
|---|---|---|
| P1-01 | `tray::refresh` always runs on the main thread (inline there, queued elsewhere), so the tray lock is never held while waiting for the main thread. | `tray.rs` |
| P1-02 | `derive_kek` rejects Argon2 costs outside m 8 MiB–1 GiB, t 1–16, p 1–16 (covers backups and the DB). | `vault/crypto.rs` |
| P2-01 | TOTP honours `digits` (6–8), `period` (1–300 s), `algorithm` (SHA1/256/512) and `%XX` values; unsupported links are rejected. UI countdown and grouping follow the period and digit count. | `vault/totp.rs`, `commands.rs`, `api.ts`, `EntryDetail.tsx` |
| P2-02 | Saving an entry that was deleted meanwhile re-inserts it instead of reporting success. | `vault/mod.rs` |
| P2-03 | Vault import runs in one transaction. | `vault/mod.rs` |
| P2-04 | Unlock / setup / change password: metadata read under the lock, Argon2 outside it; setup uses `INSERT` (second setup fails), password change is compare-and-swap. | `vault/mod.rs`, `commands.rs` |
| P2-05 | Clipboard-image thumbnails come from the RGBA buffer (borrowed, no copy). | `history.rs`, `watcher.rs` |
| P3-01 | TOTP fetched once per period instead of every second. | `EntryDetail.tsx` |
| P3-02 | "Dashes" off now gives words with no separator. | `password.ts` |
| P3-03 | All `lock().unwrap()` recover poisoned guards. | 7 files |
| P3-04 | Pins import: only text/file pins with content, one transaction. | `commands.rs` |

## 7. Verification

- `cargo test`: **18 passed** (12 existing + 6 new: `backup_rejects_absurd_kdf_params`, `save_recreates_missing_entry`,
  `import_is_all_or_nothing`, `second_setup_and_stale_password_change_fail`, `rfc6238_vectors` (SHA1/256/512),
  `parses_raw_and_otpauth`, `thumb_from_rgba_keeps_aspect_ratio`; the old single TOTP test was replaced).
- `cargo clippy --all-targets`: same 3 pre-existing style warnings, none new.
- `npm run build` (tsc + vite): passes.
- Generator checked by running `password.ts`: dashes on → `ease-folk-east-hand-skip`, off → `toolloopwaxkingmind`.
- Measurements (release profile, opt-level "s", this Mac): Argon2 derive 125 ms (now outside the DB lock);
  5K screenshot thumbnail 243 ms → 96 ms upper bound for the new path.
- **Not verified in the running app:** the tray deadlock fix (needs a GUI runtime and an unlucky interleaving to
  reproduce), the TOTP detail view, and thumbnails during live capture. No `tauri build` release bundle was produced.

## 8. Follow-up fixes (R-01 to R-04)

| ID | Change | Verification |
|---|---|---|
| R-01 | Linux: the change counter comes from XFixes `SelectionNotify` events on CLIPBOARD (x11rb, already in the dependency tree), so polling is an atomic load. Own writes wait (≤250 ms, measured 3.7 ms) for their own event so the watcher still skips them. Falls back to fingerprinting if XFixes is unavailable or the X connection drops. | Docker (Debian, Xvfb), release build, 4K image on the clipboard, 20 s: **old** 2 polls (each read blocked ~10 s), 0.77 s CPU (3.9% of a core); **new** 57 polls, 0.01 s CPU (0.1%). External copies counted (+2 for 2 copies), own write recognised. Full-app `cargo check` on Linux: see below. |
| R-02 | The verified update package is written to `<cache>/updates/` and only its SHA-256 stays in memory; `install` re-reads it and refuses a changed file (forcing a fresh, signature-checked download). Leftovers are deleted at startup and when a newer release replaces the pending one. | Test `package_on_disk_round_trips_and_detects_changes`. Not exercised against a real release. |
| R-03 | CSP: `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' asset: http://asset.localhost data:; connect-src ipc: http://ipc.localhost; object-src 'none'; base-uri 'none'; form-action 'none'`. `devCsp: null` keeps `tauri dev` (Vite HMR, inline preamble) working. | Config accepted and embedded by tauri-build; bundle has no inline scripts, `eval` or external URLs, and the one `createElement("style")` is React's unused `<style precedence>` path. **Not checked in a running release build.** |
| R-04 | Touch ID key stored in the data protection keychain with `BiometryCurrentSet` access control when the build allows it, read with the `LAContext` of the Touch ID check that just passed (no second prompt). Ad-hoc builds get `errSecMissingEntitlement` (-34018, probed on this Mac) and fall back to the previous app-gated item; reads fall back on -25300/-34018. Existing users keep their stored key. | Fallback codes probed with an unsigned and an ad-hoc binary. **The biometry-bound path cannot run until the app is signed with a Developer ID and a `keychain-access-groups` entitlement**, so it is untested. |
