package main

import (
	"context"

	"github.com/EnclaveRunner/runner/config"
	pb "github.com/EnclaveRunner/runner/proto_gen"
	"github.com/EnclaveRunner/runner/queue"
	"github.com/EnclaveRunner/shareddeps"
	shareddepsConfig "github.com/EnclaveRunner/shareddeps/config"
	"google.golang.org/grpc"
)

func main() {
	ctx := context.Background()

	cfg := &config.AppConfig{}

	server := initialize(ctx, cfg, "main")

	listen(cfg, server)
}

func initialize(
	ctx context.Context,
	cfg *config.AppConfig,
	topic string,
) *grpc.Server {
	// Set configuration defaults
	defaults := []shareddepsConfig.DefaultValue{
		//nolint:mnd // Default port of postgres
		{Key: "redis.port", Value: 6379},
		{Key: "redis.host", Value: "redis"},
		{Key: "redis.db", Value: 0},
		{Key: "runners", Value: 1},
		{Key: "artifact_registry.host", Value: "artifactregistry"},
		//nolint:mnd // Default port of artifact registry
		{Key: "artifact_registry.port", Value: 5000},
	}

	shareddeps.PopulateAppConfig(cfg, "runner", "v0.1.1", defaults...)
	server := shareddeps.InitGRPCServer()

	registryClient := pb.NewRegistryServiceClient(
		shareddeps.InitGRPCClient(
			cfg.ArtifactRegistry.Host,
			cfg.ArtifactRegistry.Port,
		),
	)

	queue.InitQueueConnection(ctx, cfg, registryClient, topic)

	return server
}

func listen(cfg *config.AppConfig, server *grpc.Server) {
	shareddeps.StartGRPCServer(cfg, server)
}
