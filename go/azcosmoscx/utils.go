// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

package azcosmoscx

import (
	"runtime"
	"unsafe"
)

// #cgo CFLAGS: -I${SRCDIR}/include
// #include <cosmoscx.h>
import "C"

func makeStr(s string) C.CosmosCxStr {
	ptr := unsafe.StringData(s)
	return C.CosmosCxStr{
		data: (*C.uint8_t)(ptr),
		len:  C.uintptr_t(len(s)),
	}
}

func makeStrPinned(s string, pin *runtime.Pinner) C.CosmosCxStr {
	ptr := unsafe.StringData(s)
	pin.Pin(ptr)
	return C.CosmosCxStr{
		data: (*C.uint8_t)(ptr),
		len:  C.uintptr_t(len(s)),
	}
}
