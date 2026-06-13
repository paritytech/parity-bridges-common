FROM node:22-alpine

WORKDIR /app

COPY package.json ./
# No dependencies to install — stdlib only

COPY src/ ./src/

EXPOSE 3000

CMD ["node", "src/index.js"]
