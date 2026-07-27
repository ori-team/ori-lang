const MASK = (1n << 64n) - 1n;
const n: number = 20_000_000;
let a = 0n;
let b = 1n;
let i = 0;
while (i < n) {
  const t = (a + b) & MASK;
  a = b;
  b = t;
  i = i + 1;
}
let out = a;
if (out >= 1n << 63n) out -= 1n << 64n;
console.log(out.toString());
export {};
