# Fixed-width wrap (like i64) so Python does not grow into bigints.
# Matches wrapping add on Ori/Rust 64-bit integers.
MASK = (1 << 64) - 1
n = 20_000_000
a, b = 0, 1
i = 0
while i < n:
    a, b = b, (a + b) & MASK
    i += 1
# signed i64 print for result parity with wrapping languages
if a >= (1 << 63):
    a -= 1 << 64
print(a)
