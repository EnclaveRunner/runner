package main

import (
	"context"

	"github.com/EnclaveRunner/runner/config"
	"github.com/EnclaveRunner/runner/queue"
	"github.com/EnclaveRunner/shareddeps"
	shareddepsConfig "github.com/EnclaveRunner/shareddeps/config"
)

func main() {
	ctx := context.Background()

	initialize(ctx, "main")

	listen()
}

func initialize(ctx context.Context, topic string) {
	// Set configuration defaults
	defaults := []shareddepsConfig.DefaultValue{
		//nolint:mnd // Default port of postgres
		{Key: "redis.port", Value: 6379},
		{Key: "redis.host", Value: "redis"},
		{Key: "redis.db", Value: 0},
		{Key: "runners", Value: 1},
	}

	cfg := &config.AppConfig{}

	shareddeps.InitGRPCServer(cfg, "runner", "v0.0.0", defaults...)

	queue.InitQueueConnection(ctx, *cfg, topic)
}

func listen() {
	shareddeps.StartGRPCServer()
}
