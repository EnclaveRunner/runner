package queue

import (
	"context"

	"github.com/hibiken/asynq"
	"github.com/rs/zerolog/log"
)

func processJob(ctx context.Context, t *asynq.Task) error {
	msg := string(t.Payload())
	log.Info().Str("payload", msg).Msg("Processing message")

	return nil
}
