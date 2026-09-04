package main

import "fmt"

func main() {
	ch := make(chan int, 1)
	count := 0
	for i := 0; i < 100000; i++ {
		ch <- i
		v := <-ch
		if v == i {
			count++
		}
	}
	close(ch)
	fmt.Println(count)
}
