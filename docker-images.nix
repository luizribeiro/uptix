{
  homeAssistant = docknix.image "ghcr.io/home-assistant/home-assistant:stable";
  appdaemon = docknix.image "acockburn/appdaemon:latest";
  zwavejs2mqtt = docknix.image "zwavejs/zwavejs2mqtt:latest";
  shlink = docknix.image "shlinkio/shlink:stable";
  shlinkWebClient = docknix.image "shlinkio/shlink-web-client:stable";
  dockerOSX = docknix.image "sickcodes/docker-osx:auto";
  prowlarr = docknix.image "linuxserver/prowlarr:nightly";
  minecraftBedrockServer = docknix.image "itzg/minecraft-bedrock-server:latest";
  ipv6nat = docknix.image "robbertkl/ipv6nat:latest";
  dokku = docknix.image "dokku/dokku:latest";
  githubRunner = docknix.image "myoung34/github-runner:latest";
}
