FROM node:12 as build-deps

ENV SUBSTRATE_PROVIDER wss://wss.rialto.brucke.link
ENV ETHEREUM_PROVIDER http://rpc.rialto.brucke.link:8545
ENV EXPECTED_ETHEREUM_NETWORK_ID 105

# install tools and dependencies
RUN set -eux; \
	apt-get install -y git

# clone UI repo
RUN cd /usr/src/ && git clone https://github.com/paritytech/bridge-ui.git
WORKDIR /usr/src/bridge-ui
RUN yarn
RUN yarn build:docker

# Stage 2 - the production environment
FROM nginx:1.12
COPY nginx/*.conf /etc/nginx/conf.d/
COPY /usr/src/app/bridge-ui/dist /usr/share/nginx/html
EXPOSE 80
CMD ["nginx", "-g", "daemon off;"]
