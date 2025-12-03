
# Build runner executable
FROM golang:1.25-alpine AS builder

WORKDIR /app

COPY . .

RUN go mod download && CGO_ENABLED=0 GOOS=linux go build -o /app/runner .

# Create a minimal runtime image
FROM alpine:3.23

RUN apk --no-cache add ca-certificates
WORKDIR /app

COPY --from=builder /app/runner .

EXPOSE 8080

CMD ["./runner"]