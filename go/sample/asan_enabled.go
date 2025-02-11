//go:build asan

package main

// #cgo CFLAGS: -fsanitize=address
// #cgo LDFLAGS: -fsanitize=address
// void __lsan_do_leak_check(void);
import "C"

func doLeakCheck() {
	C.__lsan_do_leak_check()
}
