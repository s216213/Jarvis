"""CPU Kokoro server. OpenAI-compatible POST /v1/audio/speech, WAV out."""

import io
import os
from contextlib import asynccontextmanager

import numpy as np
import soundfile as sf
from fastapi import FastAPI, HTTPException
from fastapi.responses import Response
from pydantic import BaseModel, Field

os.environ.setdefault("HF_HOME", "/var/lib/kokoro/hf")

_pipelines: dict = {}


def lang_for(voice: str) -> str:
    if voice.startswith(("bm_", "bf_")):
        return "b"
    return "a"


def pipeline(lang: str):
    if lang not in _pipelines:
        from kokoro import KPipeline

        _pipelines[lang] = KPipeline(lang_code=lang)
    return _pipelines[lang]


@asynccontextmanager
async def lifespan(_app: FastAPI):
    pipeline("b")
    yield


app = FastAPI(lifespan=lifespan)


class SpeechRequest(BaseModel):
    model: str = "kokoro"
    input: str
    voice: str = "bm_george"
    response_format: str = "wav"
    speed: float = Field(default=0.92, ge=0.25, le=4.0)


@app.get("/health")
def health() -> dict:
    return {"ok": True}


@app.post("/v1/audio/speech")
def speech(body: SpeechRequest) -> Response:
    text = body.input.strip()
    if not text:
        raise HTTPException(status_code=400, detail="input is required")
    if body.response_format != "wav":
        raise HTTPException(status_code=400, detail="only wav is supported")

    chunks = []
    try:
        for _graphemes, _phonemes, audio in pipeline(lang_for(body.voice))(
            text,
            voice=body.voice,
            speed=body.speed,
        ):
            if audio is not None and len(audio):
                chunks.append(np.asarray(audio, dtype=np.float32))
    except Exception as exc:
        raise HTTPException(status_code=502, detail=f"synthesis failed: {exc}") from exc

    if not chunks:
        raise HTTPException(status_code=502, detail="synthesis returned no audio")

    buffer = io.BytesIO()
    sf.write(buffer, np.concatenate(chunks), 24000, format="WAV", subtype="PCM_16")
    return Response(content=buffer.getvalue(), media_type="audio/wav")
