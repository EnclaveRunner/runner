package main

import (
	"context"
	"errors"
	"os"
	"testing"
	"time"

	pb "github.com/EnclaveRunner/runner/proto_gen"
	"github.com/EnclaveRunner/runner/queue"
	"github.com/hibiken/asynq"
	"github.com/rs/zerolog/log"
	"github.com/stretchr/testify/assert"
	"google.golang.org/protobuf/proto"
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

func TestHelloWorldTask(t *testing.T) {
	t.Parallel()
	taskData := pb.Task{
		Artifact: &pb.ArtifactIdentifier{
			Fqn: &pb.FullyQualifiedName{
				Source: "local",
				Author: "marvin",
				Name:   "hello-world",
			},
			Identifier: &pb.ArtifactIdentifier_VersionHash{
				VersionHash: "2a25e1f8f0aa9571689513d5b68c8bb94b9bc8f5a9229a8c0250482cfb1c8a99",
			},
		},
		Function: "respond",
		Input:    []byte("Test payload"),
	}

	payload, err := proto.Marshal(&taskData)

	assert.NoError(t, err)

	task := asynq.NewTask(queue.TaskTypeNormal, payload)
	taskInfo, err := client.Enqueue(task)

	assert.NoError(t, err)

	for {
		live, err := inspector.GetTaskInfo(taskInfo.Queue, taskInfo.ID)

		if errors.Is(err, asynq.ErrTaskNotFound) {
			break
		}

		assert.NoError(t, err)

		log.Info().
			Str("state", live.State.String()).
			Msg("Task is still processing...")

		time.Sleep(100 * time.Millisecond)
	}
}
