//go:build azcosmoscx_local && !dynamic && windows && arm64

package azcosmoscx

// #cgo debug LDFLAGS: ${SRCDIR}/../../artifacts/x86_64-pc-windows-msvc/debug/lib/cosmoscx.lib
// #cgo !debug LDFLAGS: ${SRCDIR}/../../artifacts/x86_64-pc-windows-msvc/release/lib/cosmoscx.lib
// #include <cosmoscx.h>
import "C"
