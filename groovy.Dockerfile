FROM ubuntu:focal

RUN apt-get update \
        && apt-get -y install tar cron zip software-properties-common curl \
        qemu-user-static binfmt-support

# Install OpenJDK 8 (works on both arm64 and amd64)
RUN apt-get update && apt-get install -y openjdk-8-jdk

# Set up Java environment
RUN mkdir -p /usr/java/ \
    && ln -s /usr/lib/jvm/java-8-openjdk-$(dpkg --print-architecture) /usr/java/jdk1.8.0 \
    && ln -s /usr/java/jdk1.8.0 /usr/java/latest \
    && ln -s /usr/java/jdk1.8.0 /usr/lib/jvm/default-java \
    && apt clean

ENV JAVA_HOME=/usr/java/latest

ENV GROOVY_HOME /opt/groovy

RUN set -o errexit -o nounset \
    && echo "Downloading groovy-sandbox-1.6.jar" \
    && curl -fsSL https://repo1.maven.org/maven2/org/kohsuke/groovy-sandbox/1.6/groovy-sandbox-1.6.jar \
       -o /tmp/groovy-sandbox-1.6.jar \
    && echo "Copying groovy-sandbox-1.6.jar to /app/libs" \
    && mkdir -p /app/libs \
    && cp /tmp/groovy-sandbox-1.6.jar /app/libs/ \
    && echo "Adding groovy user and group" \
    && groupadd --system --gid 1000 groovy \
    && useradd --system --gid groovy --uid 1000 --shell /bin/bash --create-home groovy \
    && mkdir --parents /home/groovy/.groovy/grapes \
    && chown --recursive groovy:groovy /home/groovy \
    && chmod --recursive 1777 /home/groovy \
    \
    && echo "Symlinking root .groovy to groovy .groovy" \
    && ln --symbolic /home/groovy/.groovy /root/.groovy

VOLUME /home/groovy/.groovy/grapes

WORKDIR /home/groovy

RUN apt-get update \
    && echo "Installing build dependencies" \
    && apt-get install --yes --no-install-recommends \
        dirmngr \
        fontconfig \
        gnupg \
        unzip \
        wget \
    && rm --recursive --force /var/lib/apt/lists/*

ENV GROOVY_VERSION 2.5.13
RUN set -o errexit -o nounset \
    && echo "Downloading Groovy" \
    && wget --no-verbose --output-document=groovy.zip "https://archive.apache.org/dist/groovy/${GROOVY_VERSION}/distribution/apache-groovy-binary-${GROOVY_VERSION}.zip" \
    # https://archive.apache.org/dist/groovy/2.5.13/distribution/apache-groovy-binary-2.5.13.zip
    \
    && echo "Importing keys listed in http://www.apache.org/dist/groovy/KEYS from key server" \
    && export GNUPGHOME="$(mktemp -d)" \
    && gpg --batch --no-tty --keyserver keyserver.ubuntu.com --recv-keys \
        7FAA0F2206DE228F0DB01AD741321490758AAD6F \
        331224E1D7BE883D16E8A685825C06C827AF6B66 \
        34441E504A937F43EB0DAEF96A65176A0FB1CD0B \
        9A810E3B766E089FFB27C70F11B595CEDC4AEBB5 \
        81CABC23EECA0790E8989B361FF96E10F0E13706 \
    \
    && echo "Checking download signature" \
    && wget --no-verbose --output-document=groovy.zip.asc "https://archive.apache.org/dist/groovy/${GROOVY_VERSION}/distribution/apache-groovy-binary-${GROOVY_VERSION}.zip.asc" \
    && gpg --batch --no-tty --verify groovy.zip.asc groovy.zip \
    && rm --recursive --force "${GNUPGHOME}" \
    && rm groovy.zip.asc \
    \
    && echo "Installing Groovy" \
    && unzip groovy.zip \
    && rm groovy.zip \
    && mv "groovy-${GROOVY_VERSION}" "${GROOVY_HOME}/" \
    && ln --symbolic "${GROOVY_HOME}/bin/grape" /usr/bin/grape \
    && ln --symbolic "${GROOVY_HOME}/bin/groovy" /usr/bin/groovy \
    && ln --symbolic "${GROOVY_HOME}/bin/groovyc" /usr/bin/groovyc \
    && ln --symbolic "${GROOVY_HOME}/bin/groovyConsole" /usr/bin/groovyConsole \
    && ln --symbolic "${GROOVY_HOME}/bin/groovydoc" /usr/bin/groovydoc \
    && ln --symbolic "${GROOVY_HOME}/bin/groovysh" /usr/bin/groovysh \
    && ln --symbolic "${GROOVY_HOME}/bin/java2groovy" /usr/bin/java2groovy

USER groovy

WORKDIR /app

COPY src/Runner.groovy /app/

ENV CLASSPATH=/app/libs/groovy-sandbox-1.6.jar

CMD ["groovy", "/app/Runner.groovy"]
