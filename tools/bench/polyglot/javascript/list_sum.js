"use strict";
const n = 1_000_000;
const xs = [];
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
