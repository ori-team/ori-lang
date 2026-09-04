def intersect(a, b):
    if a[2] < b[0] or a[0] > b[2]:
        return False
    if a[3] < b[1] or a[1] > b[3]:
        return False
    return True

target = (10.0, 10.0, 20.0, 20.0)
hits = 0
for _ in range(1000000):
    probe = (15.0, 15.0, 25.0, 25.0)
    if intersect(target, probe):
        hits += 1
print(hits)
