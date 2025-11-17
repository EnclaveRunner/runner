package queue

import (
	"github.com/ThreeDotsLabs/watermill/message"
	"github.com/rs/zerolog/log"
)

func process(messages <-chan *message.Message) {
	for msg := range messages {
		// Process the message
		log.Info().Str("payload", string(msg.Payload)).Msg("Received message")

		// Acknowledge the message
		msg.Ack()
	}
}