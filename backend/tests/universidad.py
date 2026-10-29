"""Datos compartidos por las pruebas de D3 y subconsultas.

A propósito: Sofía no tiene inscripciones, Estadística no tiene inscritos,
Marta no tiene edad y una de sus notas es NULL.
"""

UNIVERSIDAD = """
CREATE TABLE estudiantes (id INT PRIMARY KEY, nombre VARCHAR(20) NOT NULL, edad INT, promedio FLOAT);
CREATE TABLE cursos (id INT PRIMARY KEY, nombre VARCHAR(20) NOT NULL, creditos INT);
CREATE TABLE inscripciones (estudiante_id INT, curso_id INT, nota FLOAT, PRIMARY KEY (estudiante_id, curso_id));
INSERT INTO estudiantes VALUES (1, 'Ana', 20, 88.5), (2, 'Luis', 17, 75.0), (3, 'Marta', NULL, 90.0), (4, 'Sofia', 22, NULL);
INSERT INTO cursos VALUES (1, 'Compiladores', 5), (2, 'Bases de datos', 4), (3, 'Redes', 3), (4, 'Estadistica', 2);
INSERT INTO inscripciones VALUES (1, 1, 91), (1, 3, 55.5), (2, 3, 80), (3, 2, NULL), (3, 1, 70);
"""
