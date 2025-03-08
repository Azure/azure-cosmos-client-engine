//go:build !azcosmoscx_local && !dynamic && windows && amd64

package azcosmoscx

// #cgo LDFLAGS: ${SRCDIR}/libcosmoscx-vendor/x86_64-pc-windows-gnu/libcosmoscx.a
// #include <cosmoscx.h>
import "C"
