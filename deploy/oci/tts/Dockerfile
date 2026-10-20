# CPU-only Kokoro. The published hwdsl2/kokoro-server:latest arm64 image
# imports a CUDA PyTorch and dies on machines without an NVIDIA GPU.
FROM python:3.12-slim-bookworm

RUN apt-get update \
    && apt-get install -y --no-install-recommends espeak-ng libsndfile1 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /opt/tts

RUN pip install --no-cache-dir --upgrade pip \
    && if ! pip install --no-cache-dir 'torch==2.6.0' --index-url https://download.pytorch.org/whl/cpu; then \
         pip install --no-cache-dir 'torch==2.5.1'; \
       fi \
    && python -c "import glob, os, torch; root=os.path.dirname(torch.__file__); bad=glob.glob(root + '/**/libcudart*', recursive=True); print('torch', torch.__version__); assert not bad, bad" \
    && pip install --no-cache-dir "kokoro>=0.9.4" soundfile fastapi "uvicorn[standard]" \
    && python -c "import glob, os, torch; root=os.path.dirname(torch.__file__); bad=glob.glob(root + '/**/libcudart*', recursive=True); print('torch', torch.__version__); assert not bad, bad"

COPY server.py /opt/tts/server.py

ENV PYTHONUNBUFFERED=1 \
    HF_HOME=/var/lib/kokoro/hf

EXPOSE 8880
VOLUME ["/var/lib/kokoro"]
CMD ["uvicorn", "server:app", "--host", "0.0.0.0", "--port", "8880"]
