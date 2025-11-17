package config

import (
	enclaveConfig "github.com/EnclaveRunner/shareddeps/config"
)

const PrimaryTopic = "main"

type AppConfig struct {
	enclaveConfig.BaseConfig `mapstructure:",squash"`

	Kafka struct {
		Host string `mapstructure:"host"     validate:"required,hostname|ip"`
		Port int    `mapstructure:"port"     validate:"required,numeric,min=1,max=65535"`
	} `mapstructure:"kafka" validate:"required"`
}

var Cfg = &AppConfig{}
