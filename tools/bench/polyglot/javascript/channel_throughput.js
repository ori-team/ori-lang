let count = 0;
let q = [];
for (let i = 0; i < 100000; i++) {
    q.push(i);
    const v = q.shift();
    if (v === i) count++;
}
console.log(count.toString());
