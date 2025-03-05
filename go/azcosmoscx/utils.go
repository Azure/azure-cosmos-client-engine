package azcosmoscx

import "unsafe"

// #cgo CFLAGS: -I${SRCDIR}/../../include
// #include <cosmoscx.h>
import "C"

func makeStr(s string) C.CosmosCxStr {
	ptr := unsafe.StringData(s)
	return C.CosmosCxStr{
		data: (*C.uint8_t)(ptr),
		len:  C.uintptr_t(len(s)),
	}
}
