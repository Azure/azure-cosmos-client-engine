//go:build !azcosmoscx_local && !dynamic && linux && amd64

package azcosmoscx

// #cgo LDFLAGS: ${SRCDIR}/libcosmoscx-vendor/x86_64-unknown-linux-gnu/libcosmoscx.a -ldl
// #include <cosmoscx.h>
import "C"
