//go:build local && !dynamic && linux && amd64

package azcosmoscx

// #cgo debug LDFLAGS: ${SRCDIR}/../../artifacts/x86_64-unknown-linux-gnu/debug/lib/libcosmoscx.a -ldl
// #cgo !debug LDFLAGS: ${SRCDIR}/../../artifacts/x86_64-unknown-linux-gnu/release/lib/libcosmoscx.a -ldl
// #include <cosmoscx.h>
import "C"
