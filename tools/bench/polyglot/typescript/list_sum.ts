const n: number = 1_000_000;
const xs: number[] = [];
let i = 0;
while (i < n) {
  xs.push(i);
  i = i + 1;
}
let s = 0;
i = 0;
while (i < xs.length) {
  s = s + xs[i];
  i = i + 1;
}
console.log(s);
export {};
