"use strict";
const n = 10_000_000;
let s = 0;
let i = 0;
while (i < n) {
  s = s + i;
  i = i + 1;
}
console.log(s);
