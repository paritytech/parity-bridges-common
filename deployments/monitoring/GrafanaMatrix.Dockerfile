FROM ruby:alpine

RUN apk add --no-cache git

ENV APP_HOME /app
ENV RACK_ENV production
RUN mkdir $APP_HOME
WORKDIR $APP_HOME

RUN git clone https://github.com/ananace/ruby-grafana-matrix.git $APP_HOME
RUN git checkout 0d662b29633d16176291d11a2d85ba5107cf7de3
RUN bundle install --without development

RUN mkdir /config && touch /config/config.yml && ln -s /config/config.yml ./config.yml

CMD ["bundle", "exec", "bin/server"]
