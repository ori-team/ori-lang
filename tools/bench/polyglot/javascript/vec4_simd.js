const v = new Float32Array([1.0, 2.0, 3.0, 4.0]);
const s0 = 0.5, s1 = 0.25, s2 = 0.125, s3 = 0.0625;
for (let i = 0; i < 5000000; i++) {
    v[0] += s0;
    v[1] += s1;
    v[2] += s2;
    v[3] += s3;
}
const sum = v[0] + v[1] + v[2] + v[3];
console.log(Number(sum.toPrecision(6)).toString());
