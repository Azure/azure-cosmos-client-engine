package engine_test

import (
	"testing"

	"github.com/Azure/azure-cosmos-client-engine/go/engine"
	"github.com/stretchr/testify/assert"
)

func TestVersion(t *testing.T) {
	version := engine.Version()
	assert.Regexp(t, `\d+\.\d+\.\d+`, version)
}
