package main

import (
	"fmt"
)

func main() {
	v0, v1, v2, v3 := float32(1.0), float32(2.0), float32(3.0), float32(4.0)
	s0, s1, s2, s3 := float32(0.5), float32(0.25), float32(0.125), float32(0.0625)
	for i := 0; i < 5000000; i++ {
		v0 += s0
		v1 += s1
		v2 += s2
		v3 += s3
	}
	fmt.Printf("%.6g\n", v0+v1+v2+v3)
}
