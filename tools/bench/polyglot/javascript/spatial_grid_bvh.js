function intersect(a, b) {
    if (a.maxX < b.minX || a.minX > b.maxX) return false;
    if (a.maxY < b.minY || a.minY > b.maxY) return false;
    return true;
}

const target = { minX: 10.0, minY: 10.0, maxX: 20.0, maxY: 20.0 };
let hits = 0;
for (let i = 0; i < 1000000; i++) {
    const probe = { minX: 15.0, minY: 15.0, maxX: 25.0, maxY: 25.0 };
    if (intersect(target, probe)) hits++;
}
console.log(hits.toString());
