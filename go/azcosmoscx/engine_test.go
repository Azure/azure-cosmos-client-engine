// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

package azcosmoscx_test

import (
	"testing"

	"github.com/Azure/azure-cosmos-client-engine/go/azcosmoscx"
	"github.com/stretchr/testify/assert"
)

func TestVersion(t *testing.T) {
	version := azcosmoscx.Version()
	assert.Regexp(t, `\d+\.\d+\.\d+`, version)
}
