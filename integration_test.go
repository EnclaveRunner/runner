package main

import (
	"context"
	"errors"
	"os"
	"strconv"
	"testing"
	"time"

	"github.com/EnclaveRunner/runner/queue"
	"github.com/hibiken/asynq"
	"github.com/rs/zerolog/log"
	"github.com/stretchr/testify/assert"
)

const topic = "testing"

var (
	client    *asynq.Client
	inspector *asynq.Inspector
)

func TestMain(m *testing.M) {
	ctx, cancel := context.WithCancel(context.Background())

	_ = os.Setenv("ENCLAVE_REDIS_HOST", "localhost")
	_ = os.Setenv("ENCLAVE_REDIS_PORT", "6379")
	_ = os.Setenv("ENCLAVE_REDIS_DB", "0")

	redisOpt := asynq.RedisClientOpt{
		Addr: "localhost:6379",
		DB:   0,
	}

	client = asynq.NewClient(redisOpt)
	inspector = asynq.NewInspector(redisOpt)

	initialize(ctx, topic)

	code := m.Run()

	cancel()
	_ = client.Close()

	os.Exit(code)
}

func TestPublish(t *testing.T) {
	t.Parallel()

	tasks := make([]*asynq.TaskInfo, 1000)

	for i := range 1000 {
		task := asynq.NewTask(
			queue.TaskTypeNormal,
			[]byte("test payload "+strconv.Itoa(i)),
		)
		taskInfo, err := client.Enqueue(task)

		assert.NoError(t, err)

		tasks[i] = taskInfo
	}

	allFinished := false

	for !allFinished {
		allFinished = true

		time.Sleep(100 * time.Millisecond)

		for _, taskInfo := range tasks {
			live, err := inspector.GetTaskInfo(taskInfo.Queue, taskInfo.ID)
			if err != nil {
				log.Debug().
					Str("payload", string(live.Payload)).
					Str("state", live.State.String()).
					Msg("Task not yet completed")
				allFinished = false

				break
			}

			if !errors.Is(err, asynq.ErrTaskNotFound) {
				assert.NoError(t, err)
			}
		}
	}

	log.Info().Msg("All tasks have been processed")
}
