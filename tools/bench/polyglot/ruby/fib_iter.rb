# 64-bit wrap (like i64) — avoid Ruby bigint blow-up dominating the kernel.
MASK = (1 << 64) - 1
n = 20_000_000
a = 0
b = 1
i = 0
while i < n
  t = (a + b) & MASK
  a = b
  b = t
  i = i + 1
end
a -= (1 << 64) if a >= (1 << 63)
puts a
