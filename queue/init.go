package queue

import (
	"context"
	"fmt"

	"github.com/EnclaveRunner/runner/config"
	"github.com/hibiken/asynq"
	"github.com/rs/zerolog/log"
)

const TaskTypeNormal = "job:normal"

func InitQueueConnection(
	ctx context.Context,
	cfg config.AppConfig,
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
				"critical": 6,
				"default":  3,
				"low":      1,
			},
		},
	)

	mux := asynq.NewServeMux()

	mux.HandleFunc(TaskTypeNormal, processJob)

	go func() {
		log.Info().Msg("Starting Asynq server...")
		if err := srv.Run(mux); err != nil {
			log.Fatal().Err(err).Msg("Could not run Asynq server")
		}
	}()
}
