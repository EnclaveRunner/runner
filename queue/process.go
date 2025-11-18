package queue

import (
	"github.com/ThreeDotsLabs/watermill/message"
	"github.com/rs/zerolog/log"
)

func process(messages <-chan *message.Message, id int) {
	log.Info().Msg("Started processing messages")
	for msg := range messages {
		// Process the message
		log.Info().
			Str("payload", string(msg.Payload)).
			Int("pid", id).
			Msg("Received message")

		// Acknowledge the message
		msg.Ack()
	}
}
