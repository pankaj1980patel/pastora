const WORDS = (
  "able acid aged also area army away baby back bake ball band bank base bath bean bear beat bell belt best bird blow blue boat body bone book boot born boss both bowl bulk burn bush busy cake call calm camp card care cart case cash cast cell chat chef chip city clay clip club coal coat code coin cold cook cool copy corn cost crew crop dark data dawn deal deep deer desk dial dish dock door dove draw drum duck dust duty each earn ease east echo edge epic even exit face fact fair fall farm fast fern film fire fish five flag flat flow foam fold folk font food foot fork form fort four free frog fuel full fund gain game gate gear gift glow goal gold golf good grab gray grid grow gulf half hall hand harp hawk heat herb hero hill hint hold home hood hook hope horn host hour huge idea inch iron item jade jazz join joke jump june jury keen kelp kind king kite knee knot lake lamp land lane last lava lawn leaf lens life lift lime line link lion list loaf lock loft logo long loop lord luck lung main mall mango maple mark mask mate maze meal meet melt menu mild milk mind mint mist mode moon moss moth move much nail name navy neat neck nest news next nice node noon nose note oak oath ocean open oval oven pace pack page palm park path peak pear pine pink pipe plan play plum poem pond pony pool port pose pour quiz race rail rain ramp rare reed reef rice ride ring rise road rock roof room root rope rose ruby rule safe sage sail salt sand seal seed shed ship shoe silk sing sink site size skip sled slow snow soap sock sofa soil song soup spin star stem step stew sun surf swan tail tank tape task teal tent tide tile time tiny toad tone tool tour town tree trim tube tuna twin unit vase vast vest view vine volt wade wall wave wax west whip wild wind wing wire wise wolf wood wool yard yarn year yoga zero zinc zone"
).split(" ");

function randInt(max: number): number {
  const buf = new Uint32Array(1);
  const limit = Math.floor(0x100000000 / max) * max;
  do crypto.getRandomValues(buf);
  while (buf[0] >= limit);
  return buf[0] % max;
}

export interface GenOptions {
  mode: "random" | "words";
  length: number;
  symbols: boolean;
  numbers: boolean;
}

export function generatePassword(o: GenOptions): string {
  if (o.mode === "words") {
    const n = Math.max(3, Math.min(10, o.length));
    const words = Array.from({ length: n }, () => WORDS[randInt(WORDS.length)]);
    if (o.numbers) words[randInt(n)] += String(randInt(10));
    return words.join(o.symbols ? "-" : " ").replace(/ /g, "-");
  }
  const lower = "abcdefghijkmnopqrstuvwxyz";
  const upper = "ABCDEFGHJKLMNPQRSTUVWXYZ";
  const digits = "23456789";
  const syms = "!@#$%^&*-_=+?";
  const sets = [lower, upper, ...(o.numbers ? [digits] : []), ...(o.symbols ? [syms] : [])];
  const all = sets.join("");
  const len = Math.max(8, Math.min(64, o.length));
  const chars = sets.map((s) => s[randInt(s.length)]); // at least one of each set
  while (chars.length < len) chars.push(all[randInt(all.length)]);
  for (let i = chars.length - 1; i > 0; i--) {
    const j = randInt(i + 1);
    [chars[i], chars[j]] = [chars[j], chars[i]];
  }
  return chars.join("");
}

/** Rough strength: 0 (weak) – 4 (strong). */
export function strength(pw: string): number {
  if (!pw) return 0;
  let pool = 0;
  if (/[a-z]/.test(pw)) pool += 26;
  if (/[A-Z]/.test(pw)) pool += 26;
  if (/\d/.test(pw)) pool += 10;
  if (/[^a-zA-Z0-9]/.test(pw)) pool += 20;
  const bits = pw.length * Math.log2(Math.max(pool, 2));
  return bits < 35 ? 0 : bits < 50 ? 1 : bits < 65 ? 2 : bits < 85 ? 3 : 4;
}
