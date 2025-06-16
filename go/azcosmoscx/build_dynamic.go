// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//go:build dynamic && !windows

package azcosmoscx

// #cgo pkg-config: cosmoscx
// #include <cosmoscx.h>
import "C"
