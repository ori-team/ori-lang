package main

import "fmt"

func main() {
	buf := make([]byte, 64*1024)
	offset := 0
	count := 0
	total := 0
	for frame := 0; frame < 100000; frame++ {
		offset = 0
		count = 0
		_ = buf
		_ = offset
		total += count
	}
	fmt.Println(total)
}
