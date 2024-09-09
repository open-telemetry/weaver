# How to use Weaver's Docker image

![Docker Image Version](https://img.shields.io/docker/v/otel/weaver?sort=semver&label=Latest%20docker%20image%20version)

Weaver provides a [docker image](https://hub.docker.com/r/otel/weaver) for development purposes.  However, as most docker
containers being used for Development, some care must be taken in setting up the development environment. This guide
showcases how to safely leverage the image to leverage the local filesystem without requiring root access.

## Basic Usage - Otel Semconv Codegen

When generating code against the latest OpenTelemetry semantic conventions, we recommend running the docker image as follows:

```sh
docker run --rm \
        -u $(id -u ${USER}):$(id -g ${USER}) \
        --env HOME=/tmp/weaver \
        --mount 'type=bind,source=$(HOME)/.weaver,target=/tmp/weaver/.weaver' \
        --mount 'type=bind,source=$(PWD)/templates,target=/home/weaver/templates,readonly' \
        --mount 'type=bind,source=$(PWD)/src,target=/home/weaver/target' \
        otel/weaver:latest \
        registry generate \
        --templates=/home/weaver/templates \
        --target=markdown \
        /home/weaver/target
```

This has four key components:

- Using your local user as the docker container user (`-u $(id -u ${USER}):$(id -g ${USER})`)
- Binding your local codegen templates as readonly (`--mount 'type=bind,source=$(PWD)/templates,target=/home/weaver/templates,readonly'`)
- Binding the directory where code will be generated to the `/home/weaver/target` directory in the container: (` --mount 'type=bind,source=$(PWD)/src,target=/home/weaver/target'`)
- Granting weaver usage of your `~/.weaver` directory: (`--env HOME=/tmp/weaver --mount 'type=bind,source=$(HOME)/.weaver,target=/tmp/weaver/.weaver'`)

## Advanced Usage - Interactive Shell

Weaver comes with an interactive component which can be leveraged with docker.  Simply run the container with an interactive terminal attached:

```sh
docker run -it \
  otel/weaver:latest \
  registry search
```

## Advanced Usage - Policies

When enforcing policies or verifying up-to-date documentation with `weaver registry update-markdown --dry-run`, we recommend using readonly mounts within docker:

```sh
	docker run --rm \
        -u $(id -u ${USER}):$(id -g ${USER}) \
        --mount 'type=bind,source=$(PWD)/my-policy-directory,target=/home/weaver/policies,readonly' \
        --mount 'type=bind,source=$(PWD)/my-schema-yaml-directory,target=/home/weaver/source,readonly' \
        otel/weaver:latest registry check \
        --registry=/home/weaver/source \
        --policy=/home/weaver/policies
```

or for checking markdown output:

```sh
docker run --rm \
        -u $(id -u ${USER}):$(id -g ${USER}) \
        --mount 'type=bind,source=$(PWD)/templates,target=/home/weaver/templates,readonly' \
        --mount 'type=bind,source=$(PWD)/my-doc-output,target=/home/weaver/target,readonly' \
        otel/weaver:latest \
        registry generate \
        --templates=/home/weaver/templates \
        --target=markdown \
        --dry-run \
        /home/weaver/target
```

Notice in both cases, the docker image is mounting local directories as readonly.
