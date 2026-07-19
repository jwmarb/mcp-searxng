FROM python:3.12-slim-bullseye

RUN pip install uv

COPY . .

RUN uv venv --python 3.12
RUN uv sync

RUN apt-get update && apt-get install -y --no-install-recommends \
  curl ca-certificates gnupg \
  && curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
  && apt-get install -y nodejs \
  && apt-get clean && rm -rf /var/lib/apt/lists/*

RUN npm install -g @playwright/mcp@latest \
  && npx playwright install --with-deps chromium

ENTRYPOINT [ "uv", "run", "-m", "app" ]