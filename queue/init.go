package queue

import (
	"context"
	"os"

	"github.com/EnclaveRunner/runner/config"
	"github.com/ThreeDotsLabs/watermill"
	"github.com/ThreeDotsLabs/watermill-kafka/v3/pkg/kafka"
	"github.com/rs/zerolog/log"
)

func InitQueueConnection() {
	clientName, err := os.Hostname()
	if err != nil {
		log.Warn().Err(err).Msg("failed to get hostname, using default client name")
		clientName = "runner"
	}

	saramaConfig := kafka.DefaultSaramaSubscriberConfig()
	saramaConfig.ClientID = clientName

	subscriber, err := kafka.NewSubscriber(
		kafka.SubscriberConfig{
			Brokers:               []string{config.Cfg.Kafka.Host + ":" + string(rune(config.Cfg.Kafka.Port))},
			Unmarshaler:           kafka.DefaultMarshaler{},
			OverwriteSaramaConfig: saramaConfig,
		},
		watermill.NewStdLogger(true, true),
	)

	if err != nil {
		log.Fatal().Err(err).Msg("failed to create Kafka subscriber")
	}

	messages, err := subscriber.Subscribe(context.TODO(), config.PrimaryTopic)
	if err != nil {
		log.Fatal().Err(err).Msg("failed to subscribe to topic")
	}

	go process(messages)
}
