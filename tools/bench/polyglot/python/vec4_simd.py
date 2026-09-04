import struct

v = [1.0, 2.0, 3.0, 4.0]
s0, s1, s2, s3 = 0.5, 0.25, 0.125, 0.0625
for _ in range(5000000):
    v[0] += s0
    v[1] += s1
    v[2] += s2
    v[3] += s3

print(f"{sum(v):.6g}")
