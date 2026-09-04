package main

import "fmt"

type AABB struct {
	minX, minY, maxX, maxY float64
}

func intersect(a, b AABB) bool {
	if a.maxX < b.minX || a.minX > b.maxX {
		return false
	}
	if a.maxY < b.minY || a.minY > b.maxY {
		return false
	}
	return true
}

func main() {
	target := AABB{10.0, 10.0, 20.0, 20.0}
	hits := 0
	for i := 0; i < 1000000; i++ {
		probe := AABB{15.0, 15.0, 25.0, 25.0}
		if intersect(target, probe) {
			hits++
		}
	}
	fmt.Println(hits)
}
