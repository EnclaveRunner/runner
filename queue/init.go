package queue

import (
	"context"
	"fmt"
	"os"

	"github.com/EnclaveRunner/runner/config"
	"github.com/ThreeDotsLabs/watermill"
	"github.com/ThreeDotsLabs/watermill-redisstream/pkg/redisstream"
	"github.com/redis/go-redis/v9"
	"github.com/rs/zerolog/log"
)

func InitQueueConnection(
	ctx context.Context,
	cfg config.AppConfig,
	topic string,
) {
	clientName, err := os.Hostname()
	if err != nil {
		log.Warn().Err(err).Msg("failed to get hostname, using default client name")
		clientName = "runner"
	}

	subscriber, err := redisstream.NewSubscriber(
		redisstream.SubscriberConfig{
			ConsumerGroup: "enclave_runner_group",
			Unmarshaller:  redisstream.DefaultMarshallerUnmarshaller{},
			Client: redis.NewClient(&redis.Options{
				Addr:       fmt.Sprintf("%s:%d", cfg.Redis.Host, cfg.Redis.Port),
				DB:         cfg.Redis.DB,
				Username:   cfg.Redis.Username,
				Password:   cfg.Redis.Password,
				ClientName: clientName,
			}),
		},
		watermill.NewStdLogger(true, true),
	)
	if err != nil {
		log.Fatal().Err(err).Msg("failed to create subscriber")
	}

	messages, err := subscriber.Subscribe(ctx, topic)
	if err != nil {
		log.Fatal().Err(err).Msg("failed to subscribe to topic")
	}

	log.Info().Str("topic", topic).Msg("Subscribed to topic")

	for i := range cfg.Runners {
		go process(messages, i)
	}
}
