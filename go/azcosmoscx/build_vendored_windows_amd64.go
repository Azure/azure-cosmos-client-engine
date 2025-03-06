//go:build !azcosmoscx_local && !dynamic && windows && amd64

package azcosmoscx

// #cgo LDFLAGS: ${SRCDIR}/libcosmoscx-vendor/x86_64-pc-windows-msvc/libcosmoscx.a -ldl
// #include <cosmoscx.h>
import "C"
