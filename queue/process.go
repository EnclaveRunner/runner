package queue

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"io"

	pb "github.com/EnclaveRunner/runner/proto_gen"
	extism "github.com/extism/go-sdk"
	"github.com/rs/zerolog"
	"github.com/rs/zerolog/log"

	"google.golang.org/protobuf/proto"

	"github.com/hibiken/asynq"
)

type NormalTaskProcessor struct {
	registryClient pb.RegistryServiceClient
}

func (processor *NormalTaskProcessor) ProcessTask(
	ctx context.Context,
	task *asynq.Task,
) error {
	// Add task ID to log context if available
	logCtx := log.With()
	if id, ok := asynq.GetTaskID(ctx); ok {
		logCtx = logCtx.Str("taskID", id)
	}

	taskLogger := logCtx.Logger()

	payload := &pb.Task{}
	err := proto.Unmarshal(task.Payload(), payload)
	if err != nil {
		taskLogger.Error().Err(err).Msg("Failed to unmarshal task payload")

		return &InvalidPayloadError{
			Msg:   "Failed to unmarshal task payload",
			Inner: err,
		}
	}

	if payload.Artifact == nil ||
		payload.Artifact.Fqn == nil ||
		payload.Artifact.Fqn.Source == "" ||
		payload.Artifact.Fqn.Author == "" ||
		payload.Artifact.Fqn.Name == "" ||
		payload.Artifact.Identifier == nil {
		taskLogger.Error().Msg("Invalid task payload: missing required fields")

		return &InvalidPayloadError{
			Msg:   "Invalid task payload: missing required fields",
			Inner: nil,
		}
	}

	if payload.Artifact.GetVersionHash() == "" &&
		payload.Artifact.GetTag() == "" {
		taskLogger.Error().
			Msg("Invalid task payload: missing identifier (versionHash or tag)")

		return &InvalidPayloadError{
			Msg:   "Invalid task payload: missing identifier (versionHash or tag)",
			Inner: nil,
		}
	}

	logCtx = logCtx.Str("artifact", formatArtifactIdentifier(payload.Artifact))
	taskLogger = logCtx.Logger()

	taskLogger.Debug().Msg("Fetching artifact")

	wasm, err := processor.pullArtifact(ctx, &taskLogger, payload.Artifact)
	if err != nil {
		return err
	}

	manifest := extism.Manifest{
		Wasm: []extism.Wasm{
			wasm,
		},
	}

	config := extism.PluginConfig{
		EnableWasi: true,
	}
	plugin, err := extism.NewPlugin(
		ctx,
		manifest,
		config,
		[]extism.HostFunction{},
	)
	if err != nil {
		taskLogger.Error().Err(err).Msg("Failed to create Extism plugin")

		return &WasmExecutionError{
			Msg:   "Failed to create Extism plugin",
			Inner: err,
		}
	}

	taskLogger.Info().Msg("Successfully created Extism plugin")

	exit, out, err := plugin.Call(payload.Function, payload.Input)
	if err != nil {
		taskLogger.Error().Err(err).Str("function", payload.Function).Msg("Failed to call function")

		return &WasmExecutionError{
			Msg:   fmt.Sprintf("Failed to call plugin %q function", payload.Function),
			Inner: err,
		}
	}

	taskLogger.Debug().
		Str("output", string(out)).
		Msg("Plugin 'main' function output")

	if exit != 0 {
		taskLogger.Error().
			Uint32("exit_code", exit).
			Msg("Plugin 'main' function exited with non-zero code")

		return &WasmExecutionError{
			Msg:   fmt.Sprintf("Plugin 'main' function exited with code %d", exit),
			Inner: nil,
		}
	}

	return nil
}

func (processor *NormalTaskProcessor) pullArtifact(
	ctx context.Context,
	taskLogger *zerolog.Logger,
	artifact *pb.ArtifactIdentifier,
) (extism.Wasm, error) {
	taskLogger.Debug().Msg("Fetching artifact")
	stream, err := processor.registryClient.PullArtifact(ctx, artifact)
	if err != nil {
		taskLogger.Error().Err(err).Msg("Failed to initiate artifact pull")

		return nil, &PullError{
			Artifact: formatArtifactIdentifier(artifact),
			Err:      err,
		}
	}

	// Read all chunks from the stream
	var buffer bytes.Buffer
	for {
		chunk, err := stream.Recv()
		if errors.Is(err, io.EOF) {
			break
		}
		if err != nil {
			taskLogger.Error().Err(err).Msg("Failed to receive artifact chunk")

			return nil, &PullError{
				Artifact: formatArtifactIdentifier(artifact),
				Err:      err,
			}
		}

		_, _ = buffer.Write(chunk.Data)
	}

	wasmData := buffer.Bytes()

	taskLogger.Info().
		Int("size", len(wasmData)).
		Msg("Successfully pulled artifact")

	return extism.WasmData{
		Data: wasmData,
	}, nil
}

func formatArtifactIdentifier(artifact *pb.ArtifactIdentifier) string {
	var identifier string

	switch id := artifact.Identifier.(type) {
	case *pb.ArtifactIdentifier_VersionHash:
		identifier = "hash:" + id.VersionHash
	case *pb.ArtifactIdentifier_Tag:
		identifier = id.Tag
	default:
		identifier = "unknown"
	}

	return fmt.Sprintf(
		"%s/%s/%s:%s",
		artifact.Fqn.Source,
		artifact.Fqn.Author,
		artifact.Fqn.Name,
		identifier,
	)
}
