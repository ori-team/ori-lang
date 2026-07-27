const n: number = 10_000_000;
let s: number = 0;
let i: number = 0;
while (i < n) {
  s = s + i;
  i = i + 1;
}
console.log(s);
export {};
