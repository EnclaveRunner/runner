package config

import (
	enclaveConfig "github.com/EnclaveRunner/shareddeps/config"
)

type AppConfig struct {
	enclaveConfig.BaseConfig `mapstructure:",squash"`

	Runners int `mapstructure:"runners" validate:"required,numeric,min=1,max=100"`

	ArtifactRegistry struct {
		Host string `mapstructure:"host" validate:"required,hostname|ip"`
		Port int    `mapstructure:"port" validate:"required,numeric,min=1,max=65535"`
	} `mapstructure:"artifact_registry" validate:"required"`

	Redis struct {
		Host     string `mapstructure:"host"     validate:"required,hostname|ip"`
		Port     int    `mapstructure:"port"     validate:"required,numeric,min=1,max=65535"`
		DB       int    `mapstructure:"db"       validate:"numeric,min=0"`
		Username string `mapstructure:"username"`
		Password string `mapstructure:"password"`
	} `mapstructure:"redis" validate:"required"`
}
