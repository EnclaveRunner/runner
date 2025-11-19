package queue

import (
	"context"
	"fmt"

	"github.com/EnclaveRunner/runner/config"
	pb "github.com/EnclaveRunner/runner/proto_gen"
	"github.com/EnclaveRunner/shareddeps"
	"github.com/hibiken/asynq"
	"github.com/rs/zerolog/log"
)

const TaskTypeNormal = "job:normal"

const (
	taskPriorityCritical = 6
	taskPriorityDefault  = 3
	taskPriorityLow      = 1
)

func InitQueueConnection(
	ctx context.Context,
	cfg *config.AppConfig,
	topic string,
) {
	srv := asynq.NewServer(
		asynq.RedisClientOpt{
			Addr:     fmt.Sprintf("%s:%d", cfg.Redis.Host, cfg.Redis.Port),
			DB:       cfg.Redis.DB,
			Username: cfg.Redis.Username,
			Password: cfg.Redis.Password,
		},
		asynq.Config{
			Concurrency: cfg.Runners,
			BaseContext: func() context.Context {
				return ctx
			},
			Queues: map[string]int{
				"critical": taskPriorityCritical,
				"default":  taskPriorityDefault,
				"low":      taskPriorityLow,
			},
		},
	)

	mux := asynq.NewServeMux()

	shareddeps.InitGRPCClient(
		cfg.ArtifactRegistry.Host,
		cfg.ArtifactRegistry.Port,
	)

	normalTaskProcessor := &NormalTaskProcessor{
		registryClient: pb.NewRegistryServiceClient(shareddeps.GRPCClient),
	}

	mux.Handle(TaskTypeNormal, normalTaskProcessor)

	go func() {
		log.Info().Msg("Starting Asynq server...")
		if err := srv.Run(mux); err != nil {
			log.Fatal().Err(err).Msg("Could not run Asynq server")
		}
	}()
}
