package main

import (
	"context"
	"os"
	"strconv"
	"testing"

	"github.com/ThreeDotsLabs/watermill"
	"github.com/ThreeDotsLabs/watermill-redisstream/pkg/redisstream"
	"github.com/ThreeDotsLabs/watermill/message"
	"github.com/google/uuid"
	"github.com/redis/go-redis/v9"
	"github.com/rs/zerolog/log"
	"github.com/stretchr/testify/assert"
)

const topic = "testing"

var publisher *redisstream.Publisher

func TestMain(m *testing.M) {
	ctx, cancel := context.WithCancel(context.Background())

	_ = os.Setenv("ENCLAVE_REDIS_HOST", "localhost")
	_ = os.Setenv("ENCLAVE_REDIS_PORT", "6379")
	_ = os.Setenv("ENCLAVE_REDIS_DB", "0")

	var err error
	publisher, err = redisstream.NewPublisher(redisstream.PublisherConfig{
		Client: redis.NewClient(&redis.Options{
			Addr: "localhost:6379",
		}),
		Marshaller: redisstream.DefaultMarshallerUnmarshaller{},
	}, watermill.NewStdLogger(true, true))
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to create publisher")
	}

	initialize(ctx, topic)

	code := m.Run()

	cancel()

	os.Exit(code)
}

func TestPublish(t *testing.T) {
	t.Parallel()

	msgs := make([]*message.Message, 0, 1000)

	for i := range 1000 {
		msgs = append(
			msgs,
			message.NewMessage(
				uuid.NewString(),
				[]byte("test message "+strconv.Itoa(i)),
			),
		)
	}

	err := publisher.Publish(topic, msgs...)
	assert.NoError(t, err)
	
	publisher.
}
