package native_test

import (
	"testing"

	"github.com/Azure/azure-cosmos-client-engine/go/engine/native"
	"github.com/stretchr/testify/assert"
)

func TestVersion(t *testing.T) {
	version := native.EngineVersion()
	assert.Regexp(t, `\d+\.\d+\.\d+`, version)
}
