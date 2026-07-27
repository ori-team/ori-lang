const n: number = 2000;
let s = 0;
let i = 0;
while (i < n) {
  let j = 0;
  while (j < n) {
    s = s + 1;
    j = j + 1;
  }
  i = i + 1;
}
console.log(s);
export {};
