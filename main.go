package main

import (
	"github.com/EnclaveRunner/runner/config"
	"github.com/EnclaveRunner/shareddeps"
	shareddepsConfig "github.com/EnclaveRunner/shareddeps/config"
)

func main() {
	// Set configuration defaults
	defaults := []shareddepsConfig.DefaultValue{
		//nolint:mnd // Default port of postgres
		{Key: "kafka.port", Value: 9092},
		{Key: "kafka.host", Value: "kafka"},
	}
	
	shareddeps.InitGRPCServer(config.Cfg, "runner", "v0.0.0", defaults...)
	
	
	
	shareddeps.StartGRPCServer()
}